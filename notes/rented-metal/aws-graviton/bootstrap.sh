#!/bin/sh
#
# Prepare a new AWS account for the Graviton metal probe. Run once, by the account's owner, in
# AWS CloudShell, signed in as the root user (CloudShell already holds those credentials, so no key
# is typed anywhere):
#
#     sh bootstrap.sh <region> <email for budget alerts>
#
# It creates the IAM user `nife-metal` with the least-privilege policy below, turns on serial
# console access in <region>, creates a $10 monthly cost budget that emails at 25% and 100% actual
# and 100% forecast, asks for the vCPU quota a 64-vCPU metal instance needs, and last prints one
# access key for that user. Nothing here launches or costs anything. ../aws-graviton.md is the
# runbook; its section 1 says what to do with the key.
#
# Name: provisional (the AWS Graviton probe's lane, 2026-10-21 UTC), after what it does.

set -eu

[ $# -eq 2 ] || { echo "usage: sh bootstrap.sh <region> <budget email>" >&2; exit 2; }
region=$1
email=$2
user=nife-metal
account=$(aws sts get-caller-identity --query Account --output text)

# Every create action must carry the tag, and every destructive action needs it on the target, so
# this user can neither leave an untagged resource behind for teardown to miss nor touch anything
# the account owner made by hand. Reads are allowed in every region because `run check` looks for
# leftovers everywhere. The Deny holds the user to the one instance type the runbook prices.
policy=$(cat <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "ReadEverywhere",
      "Effect": "Allow",
      "Action": [
        "ec2:Describe*",
        "ec2:GetConsoleOutput",
        "ec2:GetSerialConsoleAccessStatus",
        "servicequotas:GetServiceQuota",
        "budgets:ViewBudget",
        "ssm:GetParameter"
      ],
      "Resource": "*"
    },
    {
      "Sid": "CreateOnlyTagged",
      "Effect": "Allow",
      "Action": [
        "ec2:RunInstances",
        "ec2:CreateVolume",
        "ec2:CreateSnapshot",
        "ec2:RegisterImage",
        "ec2:ImportKeyPair",
        "ec2:CreateSecurityGroup"
      ],
      "Resource": [
        "arn:aws:ec2:$region:*:instance/*",
        "arn:aws:ec2:$region:*:volume/*",
        "arn:aws:ec2:$region:*:network-interface/*",
        "arn:aws:ec2:$region::snapshot/*",
        "arn:aws:ec2:$region::image/*",
        "arn:aws:ec2:$region:*:key-pair/*",
        "arn:aws:ec2:$region:*:security-group/*"
      ],
      "Condition": {"StringEquals": {"aws:RequestTag/nife-probe": "aws-graviton"}}
    },
    {
      "Sid": "UseWhatTheCreatesReference",
      "Effect": "Allow",
      "Action": [
        "ec2:RunInstances",
        "ec2:CreateSnapshot",
        "ec2:RegisterImage",
        "ec2:CreateSecurityGroup"
      ],
      "Resource": [
        "arn:aws:ec2:$region::image/*",
        "arn:aws:ec2:$region::snapshot/*",
        "arn:aws:ec2:$region:*:volume/*",
        "arn:aws:ec2:$region:*:subnet/*",
        "arn:aws:ec2:$region:*:vpc/*",
        "arn:aws:ec2:$region:*:security-group/*",
        "arn:aws:ec2:$region:*:key-pair/*"
      ]
    },
    {
      "Sid": "TagOnlyWhileCreating",
      "Effect": "Allow",
      "Action": "ec2:CreateTags",
      "Resource": "*",
      "Condition": {"StringEquals": {"ec2:CreateAction": [
        "RunInstances", "CreateVolume", "CreateSnapshot", "RegisterImage",
        "ImportKeyPair", "CreateSecurityGroup"
      ]}}
    },
    {
      "Sid": "ActOnlyOnTagged",
      "Effect": "Allow",
      "Action": [
        "ec2:TerminateInstances",
        "ec2:StopInstances",
        "ec2:RebootInstances",
        "ec2:AttachVolume",
        "ec2:DetachVolume",
        "ec2:DeleteVolume",
        "ec2:DeleteSnapshot",
        "ec2:DeregisterImage",
        "ec2:DeleteKeyPair",
        "ec2:DeleteSecurityGroup",
        "ec2:AuthorizeSecurityGroupIngress",
        "ec2-instance-connect:SendSerialConsoleSSHPublicKey"
      ],
      "Resource": "*",
      "Condition": {"StringEquals": {"aws:ResourceTag/nife-probe": "aws-graviton"}}
    },
    {
      "Sid": "OnlyThePricedInstanceType",
      "Effect": "Deny",
      "Action": "ec2:RunInstances",
      "Resource": "arn:aws:ec2:*:*:instance/*",
      "Condition": {"StringNotEquals": {"ec2:InstanceType": "c7g.metal"}}
    },
    {
      "Sid": "RetireOwnKey",
      "Effect": "Allow",
      "Action": ["iam:ListAccessKeys", "iam:DeleteAccessKey"],
      "Resource": "arn:aws:iam::$account:user/\${aws:username}"
    }
  ]
}
EOF
)

budget='{"BudgetName":"nife-metal","BudgetType":"COST","TimeUnit":"MONTHLY",
  "BudgetLimit":{"Amount":"10","Unit":"USD"}}'
notifications=$(for n in ACTUAL:25 ACTUAL:100 FORECASTED:100; do
    printf '{"Notification":{"NotificationType":"%s","ComparisonOperator":"GREATER_THAN",' "${n%:*}"
    printf '"Threshold":%s,"ThresholdType":"PERCENTAGE"},' "${n#*:}"
    printf '"Subscribers":[{"SubscriptionType":"EMAIL","Address":"%s"}]}\n' "$email"
done | paste -sd, -)

echo "==> budget first, so it exists before anything can launch"
aws budgets create-budget --account-id "$account" --budget "$budget" \
    --notifications-with-subscribers "[$notifications]"

echo "==> serial console access in $region (an account setting, so root does it here)"
aws ec2 enable-serial-console-access --region "$region" >/dev/null

echo "==> the vCPU quota: c7g.metal is 64 vCPUs; a new account's default is 5"
aws service-quotas request-service-quota-increase --region "$region" \
    --service-code ec2 --quota-code L-1216C47A --desired-value 64 \
    --query 'RequestedQuota.Status' --output text ||
    echo "(already requested, or already at 64 or more; 'run preflight' reads the current value)"

echo "==> the IAM user $user"
aws iam create-user --user-name "$user" >/dev/null
# A managed policy, because the document is about 2,200 characters and an inline user policy
# is capped at 2,048.
aws iam create-policy --policy-name nife-metal-probe --policy-document "$policy" >/dev/null
aws iam attach-user-policy --user-name "$user" \
    --policy-arn "arn:aws:iam::$account:policy/nife-metal-probe"

echo "==> one access key for $user; paste it into 'aws configure --profile nife-metal' on the workstation"
aws iam create-access-key --user-name "$user" \
    --query 'AccessKey.[AccessKeyId,SecretAccessKey]' --output text
